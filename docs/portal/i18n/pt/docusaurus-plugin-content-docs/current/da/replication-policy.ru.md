---
lang: pt
direction: ltr
source: docs/portal/docs/da/replication-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota História Canônica
Esta página contém `docs/source/da/replication_policy.md`. Держите обе версии
:::

# Política de replicação de disponibilidade de dados (DA-4)

_Статус: В работе — Владельцы: Core Protocol WG / Storage Team / SRE_

Pipeline de ingestão DA теперь применяет детерминированные цели retenção para каждого
blob de classe, especificado em `roadmap.md` (fluxo de trabalho DA-4). Torii отказывается
envelopes de retenção сохранять, chamador предоставленные, если они не совпадают с
настроенной политикой, гарантируя, что каждый валидатор/узел хранения удерживает
Não há nenhum arquivo e uma réplica que não tenha um nome aberto.

## Política de умолчанию

| Blob de classe | Retenção quente | Retenção de frio | Réplicas de tubos | Classe хранения | Etiqueta de governança |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 dias | 3 | `cold` | `da.governance` |
| _Padrão (qualquer classe padrão)_ | 6 horas | 30 dias | 3 | `warm` | `da.default` |

Ele está localizado em `torii.da_ingest.replication_policy` e é usado
всем `/v2/da/ingest` отправкам. Torii переписывает manifestos с примененным
retenção de perfil e выдает предупреждение, если chamadores передают несоответствующие
Obviamente, todos os operadores podem usar o SDK.

### Classes de distribuição Taikai

Manifestos de roteamento Taikai (`taikai.trm`) объявляют `availability_class`
(`hot`, `warm`, ou `cold`). Torii применяет соответствующую политику до chunking,
чтобы операторы могли масштабировать число реплик по stream без редактирования
tabelas globais. Padrões:

| Classe de distribuição | Retenção quente | Retenção de frio | Réplicas de tubos | Classe хранения | Etiqueta de governança |
|-------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 dias | 3 | `cold` | `da.taikai.archive` |

Если подсказок нет, используется `hot`, чтобы transmissão ao vivo удерживали самый
perfil de construção. Переопределяйте padrões через
`torii.da_ingest.replication_policy.taikai_availability`, exceto o conjunto usado
другие цели.

##Configurações

Política de segurança para `torii.da_ingest.replication_policy` e pré-venda
*padrão* é definido como uma substituição máxima para cada classe. Identificadores de classe
registro e configuração `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` para operação, operação.
A classe de configuração é `hot`, `warm`, ou `cold`.

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

Se você definir o bloco sem nenhuma configuração, esses padrões serão usados. Чтобы ужесточить
classe, обновите соответствующий substituição; чтобы изменить базу для новых классов,
отредактируйте `default_retention`.

Aulas de disponibilidade de Taikai podem ser oferecidas
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

## Aplicação de semântica

- Torii é definido como padrão `RetentionPolicy` no perfil de usuário
  antes do chunking ou do manifesto.
- Manifestos Предсобранные, которые декларируют несовпадающий perfil de retenção,
  aberto em `400 schema mismatch`, seus clientes não podem ser acessados
  contrato.
- Каждое событие override логируется (`blob_class`, отправленная политика vs
  ожидаемая), чтобы выявлять chamadores não compatíveis no início do lançamento.

Смотрите [Plano de ingestão de disponibilidade de dados](ingest-plan.md) (Lista de verificação de validação) para
portão обновленного, retenção de aplicação покрывающего.

## Workflow повторной репликации (acompanhamento DA-4)

Retenção de execução — лишь первый шаг. Операторы также должны доказать, что live
manifestos e ordens de replicação остаются согласованными с настроенной политикой,
O SoraFS pode replicar automaticamente blobs desconhecidos.

1. **Selecione a deriva.** Torii imagem
   Codificador `overriding DA retention policy to match configured network baseline`
   chamador отправляет устаревшие retenção значения. Сопоставляйте этот лог с
   телеметрией `torii_sorafs_replication_*`, чтобы обнаружить deficiências реплик
   или задержанные reimplantações.
2. **Construa intenções e réplicas ao vivo.** Utilize o novo auxiliar de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```O comando configurou `torii.da_ingest.replication_policy` para configuração,
   decodificar o manifesto do arquivo (JSON ou Norito) e combiná-lo opcionalmente
   cargas úteis `ReplicationOrderV1` para resumo do manifesto. Aqui estão algumas situações:

   - `policy_mismatch` - manifesto de perfil de retenção расходится с принудительным
     perfil (também não é adequado, exceto Torii instalado corretamente).
   - `replica_shortfall` - pedido de replicação ao vivo запрашивает меньше реплик, чем
     `RetentionPolicy.required_replicas`, ou você está procurando algo novo, aqui está.

   Ненулевой код выхода означает активный déficit, чтобы CI/on-call автоматизация
   могла немедленно пейджить. Usando JSON do pacote
   `docs/examples/da_manifest_review_template.md` para o parâmetro global.
3. **Replicar novamente.** Não auditar o déficit, выпустите
   novo `ReplicationOrderV1` está atualizando o instrumento, analisando-o
   [Mercado de capacidade de armazenamento SoraFS](../sorafs/storage-capacity-marketplace.md),
   e повторяйте аудит, пока набор реплик не сойдется. Para substituições de emergência
   CLI é enviado para `iroha app da prove-availability`, o SRE pode ser configurado
   на тот же resumo e evidência PDP.

Cobertura de regressão fornecida em `integration_tests/tests/da/replication_policy.rs`;
suíte отправляет несовпадающую политику retenção em `/v2/da/ingest` e prova,
Este manifesto específico contém um perfil específico, um chamador sem intenção.