---
lang: pt
direction: ltr
source: docs/source/nexus_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94891050512eaf78f4c0381c0facbeed445a7e7323297070ae537e4d38ca7fe4
source_last_modified: "2025-12-13T05:07:11.953030+00:00"
translation_last_reviewed: 2026-01-01
---

# Modelo de lanes Nexus e particionamento do WSV

> **Status:** entregavel NX-1 - a taxonomia de lanes, a geometria de configuracao e o layout de storage estao prontos para implementacao.  
> **Responsaveis:** Nexus Core WG, Governance WG  
> **Item de roadmap relacionado:** NX-1

Este documento captura a arquitetura alvo da camada de consenso multi-lane do Nexus. O objetivo e produzir um unico estado mundial determinista enquanto permite que data spaces (lanes) individuais executem conjuntos de validadores publicos ou privados com workloads isolados.

> **Provas cross-lane:** Esta nota foca em geometria e storage. Os commitments de settlement por lane, o pipeline de relay e as provas de merge-ledger exigidas pelo roadmap **NX-4** estao descritas em [nexus_cross_lane.md](nexus_cross_lane.md).

## Conceitos

- **Lane:** shard logico do ledger Nexus com seu proprio conjunto de validadores e backlog de execucao. Identificada por um `LaneId` estavel.
- **Data Space:** bucket de governanca que agrupa uma ou mais lanes que compartilham politicas de compliance, roteamento e settlement. Cada dataspace tambem declara `fault_tolerance (f)` usado para dimensionar comites de relay de lane (`3f+1`).
- **Lane Manifest:** metadados controlados pela governanca descrevendo validadores, politica de DA, token de gas, regras de settlement e permissoes de roteamento.
- **Global Commitment:** prova emitida por uma lane que resume novas roots de estado, dados de settlement e transferencias cross-lane opcionais. O anel NPoS global ordena commitments.

## Taxonomia de lanes

Os tipos de lane descrevem canonicamente sua visibilidade, superficie de governanca e hooks de settlement. A geometria de configuracao (`LaneConfig`) captura esses atributos para que nos, SDKs e tooling possam raciocinar sobre o layout sem logica bespoke.

| Tipo de lane | Visibilidade | Membresia de validadores | Exposicao WSV | Governanca padrao | Politica de settlement | Uso tipico |
|-------------|-------------|--------------------------|---------------|-------------------|------------------------|-----------|
| `default_public` | publico | Permissionless (stake global) | Replica completa de estado | Parlamento SORA | `xor_global` | Ledger publico base |
| `public_custom` | publico | Permissionless ou stake-gated | Replica completa de estado | Modulo ponderado por stake | `xor_lane_weighted` | Aplicacoes publicas de alto throughput |
| `private_permissioned` | restrito | Conjunto fixo de validadores (aprovado por governanca) | Commitments e proofs | Conselho federado | `xor_hosted_custody` | CBDC, workloads de consorcio |
| `hybrid_confidential` | restrito | Membresia mista; envolve provas ZK | Commitments + divulgacao seletiva | Modulo de dinheiro programavel | `xor_dual_fund` | Dinheiro programavel com privacidade |

Todos os tipos de lane devem declarar:

- Alias de dataspace - agrupamento legivel que vincula politicas de compliance.
- Handle de governanca - identificador resolvido via `Nexus.governance.modules`.
- Handle de settlement - identificador consumido pelo settlement router para debitar buffers XOR.
- Metadados opcionais de telemetria (descricao, contato, dominio de negocio) expostos via `/status` e dashboards.

## Geometria de configuracao de lanes (`LaneConfig`)

`LaneConfig` e a geometria em runtime derivada do catalogo de lanes validado. Nao substitui manifests de governanca; em vez disso fornece identificadores de storage deterministas e hints de telemetria para cada lane configurada.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` recalcula a geometria sempre que a configuracao e carregada (`State::set_nexus`).
- Aliases sao sanitizados em slugs em minusculas; caracteres nao alfanumericos consecutivos colapsam em `_`. Se o alias gerar um slug vazio, usamos `lane{id}`.
- Prefixos de chave garantem que o WSV mantenha ranges por lane disjuntos mesmo quando o backend e compartilhado.
- `shard_id` e derivado da chave de metadata do catalogo `da_shard_id` (padrao `lane_id`) e dirige o journal persistido do cursor de shard para manter o replay de DA deterministico em reinicios/resharding.
- Nomes de segmento Kura sao deterministas entre hosts; auditores podem cruzar diretorios e manifests sem tooling bespoke.
- Segmentos merge (`lane_{id:03}_merge`) guardam as ultimas roots de merge-hint e os commitments de estado global para a lane.
- Quando a governanca renomeia um alias de lane, os nos reetiquetam automaticamente os diretorios `blocks/lane_{id:03}_{slug}` (e snapshots em camadas) para que auditores sempre vejam o slug canonico sem limpeza manual.

## Particionamento de world-state

- O world state logico do Nexus e a uniao dos espacos de estado por lane. Lanes publicas persistem estado completo; lanes privadas/confidenciais exportam roots de Merkle/commitment para o merge ledger.
- O armazenamento MV prefixa cada chave com o prefixo de 4 bytes da lane de `LaneConfigEntry::key_prefix`, produzindo chaves como `[00 00 00 01] ++ PackedKey`.
- Tabelas compartilhadas (contas, assets, triggers, registros de governanca) portanto armazenam entradas agrupadas por prefixo de lane, mantendo scans de range deterministas.
- Metadados do merge-ledger espelham o mesmo layout: cada lane escreve roots de merge-hint e roots de estado global reduzidas em `lane_{id:03}_merge`, permitindo retencao ou eviction direcionada quando uma lane se aposenta.
- Indices cross-lane (aliases de conta, registros de assets, manifests de governanca) armazenam pares explicitos `(LaneId, DataSpaceId)`. Esses indices vivem em column families compartilhadas mas usam o prefixo de lane e ids de dataspace explicitos para manter lookups deterministas.
- O workflow de merge combina dados publicos com commitments privados usando tuplas `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` derivadas de entradas do merge-ledger.

## Particionamento de Kura e WSV

- **Segmentos Kura**
  - `lane_{id:03}_{slug}` - segmento principal de blocos para a lane (blocos, indices, recibos).
  - `lane_{id:03}_merge` - segmento do merge-ledger que registra roots de estado reduzidas e artefatos de settlement.
  - Segmentos globais (evidencia de consenso, caches de telemetria) permanecem compartilhados porque sao neutros a lane; suas chaves nao incluem prefixos de lane.
- O runtime observa atualizacoes do catalogo de lanes: lanes novas tem seus diretorios de blocos e merge-ledger provisionados automaticamente em `kura/blocks/` e `kura/merge_ledger/`, enquanto lanes aposentadas sao arquivadas em `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*`.
- Snapshots de estado em camadas refletem o mesmo ciclo; cada lane escreve em `<cold_root>/lanes/lane_{id:03}_{slug}`, onde `<cold_root>` é `cold_store_root` (ou `da_store_root` quando `cold_store_root` nao esta definido), e as aposentadorias migram a arvore para `<cold_root>/retired/lanes/`.
- **Prefixos de chave** - o prefixo de 4 bytes calculado a partir de `LaneId` e sempre anexado a chaves MV codificadas. Nenhum hashing especifico de host e usado, entao a ordenacao e identica entre nos.
- **Layout do block log** - dados de bloco, indice e hashes ficam sob `kura/blocks/lane_{id:03}_{slug}/`. Jornais de merge-ledger reutilizam o mesmo slug (`kura/merge/lane_{id:03}_{slug}.log`), mantendo fluxos de recovery por lane isolados.
- **Politica de retencao** - lanes publicas retem corpos completos de blocos; lanes somente de commitments podem compactar corpos antigos apos checkpoints porque commitments sao autoritativos. Lanes confidenciais mantem jornais cifrados em segmentos dedicados para nao bloquear outras workloads.
- **Tooling** - `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` inspeciona `<store>/blocks` e `<store>/merge_ledger` usando o `LaneConfig` derivado, reporta segmentos ativos vs aposentados e arquiva diretorios/logs aposentados em `<store>/retired/...` para manter evidencia determinista. Utilitarios de manutencao (`kagami`, comandos admin CLI) devem reutilizar o namespace com slug ao expor metricas, labels Prometheus ou ao arquivar segmentos Kura.

## Orcamentos de armazenamento

- `nexus.storage.max_disk_usage_bytes` define o orcamento total em disco que nos Nexus devem consumir entre Kura, snapshots frios de WSV, armazenamento SoraFS e spools de streaming (SoraNet/SoraVPN).
- `nexus.storage.max_wsv_memory_bytes` limita a camada quente de WSV propagando o dimensionamento determinista do payload Norito em `tiered_state.hot_retained_bytes`; a retencao de graca pode exceder temporariamente o orcamento, mas o excesso e observavel via telemetria (`state_tiered_hot_bytes`, `state_tiered_hot_grace_overflow_bytes`).
- `nexus.storage.disk_budget_weights` divide o orcamento de disco entre componentes usando pontos base (deve somar 10.000). Os limites derivados se aplicam a `kura.max_disk_usage_bytes`, `tiered_state.max_cold_bytes`, `sorafs.storage.max_capacity_bytes`, `streaming.soranet.provision_spool_max_bytes` e `streaming.soravpn.provision_spool_max_bytes`.
- A aplicacao do orcamento de Kura soma os bytes do block-store em segmentos de lanes ativas e aposentadas e inclui blocos em fila ainda nao persistidos para evitar estouros durante o atraso de escrita.
- Spools de provisionamento SoraVPN usam as configuracoes `streaming.soravpn` e sao limitados de forma independente do spool de provisionamento SoraNet.
- Limites por componente ainda se aplicam: quando um componente tem um limite explicito nao zero, vale o menor entre esse limite e o orcamento Nexus derivado.
- A telemetria de orcamento usa `storage_budget_bytes_used{component=...}` e `storage_budget_bytes_limit{component=...}` para reportar uso/limites de `kura`, `wsv_hot`, `wsv_cold`, `soranet_spool` e `soravpn_spool`; `storage_budget_exceeded_total{component=...}` incrementa quando a aplicacao rejeita novos dados e os logs emitem um aviso ao operador.
- Kura reporta a mesma contabilidade usada durante a admissao (bytes em disco mais blocos em fila, incluindo payloads do merge-ledger quando presentes), entao os medidores refletem pressao efetiva e nao apenas bytes persistidos.

## Routing e APIs

- Endpoints REST/gRPC do Torii aceitam um `lane_id` opcional; ausencia implica `lane_default`.
- SDKs expoem seletores de lane e mapeiam aliases amigaveis para `LaneId` usando o catalogo de lanes.
- Regras de routing operam no catalogo validado e podem escolher lane e dataspace. `LaneConfig` fornece aliases amigaveis a telemetria para dashboards e logs.

## Settlement e fees

- Cada lane paga fees em XOR ao conjunto global de validadores. Lanes podem cobrar tokens de gas nativos mas devem escrows equivalentes XOR junto com commitments.
- Provas de settlement incluem valor, metadata de conversao e prova de escrow (ex: transferencia para o vault global de fees).
- O settlement router unificado (NX-3) debita buffers usando os mesmos prefixos de lane, para que a telemetria de settlement alinhe com a geometria de storage.

## Governanca

- Lanes declaram seu modulo de governanca via o catalogo. `LaneConfigEntry` carrega alias e slug originais para manter telemetria e trilhas de auditoria legiveis.
- O registro Nexus distribui manifests de lane assinados que incluem `LaneId`, binding de dataspace, handle de governanca, handle de settlement e metadata.
- Hooks de runtime-upgrade continuam impondo politicas de governanca (`gov_upgrade_id` por padrao) e registram diffs via ponte de telemetria (eventos `nexus.config.diff`).
- Manifests de lane definem o pool de validadores de dataspace para lanes administradas; lanes eleitas por stake derivam seu pool de validadores de registros de staking de lanes publicas.

## Telemetria e status

- `/status` expoe aliases de lane, bindings de dataspace, handles de governanca e perfis de settlement, derivados do catalogo e `LaneConfig`.
- Metricas do scheduler (`nexus_scheduler_lane_teu_*`) mostram aliases/slugs de lane para que operadores mapeiem backlog e pressao TEU rapidamente.
- `nexus_lane_configured_total` conta o numero de entradas de lane derivadas e e recalculado quando a configuracao muda. A telemetria emite diffs assinados quando a geometria de lanes muda.
- Gauges de backlog de dataspace incluem metadata de alias/descricao para ajudar operadores a associar pressao de fila com dominios de negocio.

## Configuracao e tipos Norito

- `LaneCatalog`, `LaneConfig` e `DataSpaceCatalog` vivem em `iroha_data_model::nexus` e fornecem estruturas compativeis com Norito para manifests e SDKs.
- `LaneConfig` vive em `iroha_config::parameters::actual::Nexus` e e derivado automaticamente do catalogo; nao requer encoding Norito porque e um helper interno de runtime.
- A configuracao voltada ao usuario (`iroha_config::parameters::user::Nexus`) continua aceitando descritores declarativos de lanes e dataspaces; o parsing agora deriva a geometria e rejeita aliases invalidos ou ids de lane duplicados.
- `DataSpaceMetadata.fault_tolerance` controla o tamanho do comite de relay de lane; a membresia do comite e amostrada de forma deterministica por epoca do pool de validadores do dataspace usando a semente VRF da epoca ligada a `(dataspace_id, lane_id)`.

## Trabalho pendente

- Integrar atualizacoes do settlement router (NX-3) com a nova geometria para que debitos de buffers XOR e recibos sejam etiquetados por slug de lane.
- Finalizar o algoritmo de merge (ordenacao, pruning, deteccao de conflitos) e anexar fixtures de regressao para replay cross-lane.
- Adicionar hooks de compliance para whitelists/blacklists e politicas de dinheiro programavel (rastreado em NX-12).

---

*Este documento evoluira conforme tarefas NX-2 ate NX-18 avancarem. Registre questoes em aberto no roadmap ou no tracker de governanca.*
