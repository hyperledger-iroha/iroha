---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f7fedd5b720e5e2842bc88a16be8e3c3b9e8cdd773e3d58390a2281390818b33
source_last_modified: "2025-12-09T16:53:53.145158+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-lane-model
title: Modelo de lanes do Nexus
description: Taxonomia logica de lanes, geometria de configuracao e regras de merge de world-state para Sora Nexus.
---

# Modelo de lanes do Nexus e particionamento de WSV

> **Status:** entregavel NX-1 - taxonomia de lanes, geometria de configuracao e layout de storage prontos para implementacao.  
> **Owners:** Nexus Core WG, Governance WG  
> **Referencia de roadmap:** NX-1 em `roadmap.md`

Esta pagina do portal espelha o brief canonico `docs/source/nexus_lanes.md` para que operadores do Sora Nexus, owners de SDK e revisores possam ler a guia de lanes sem entrar na arvore mono-repo. A arquitetura alvo mantem o world state determinista enquanto permite que data spaces (lanes) individuais executem conjuntos de validadores publicos ou privados com workloads isolados.

## Conceitos

- **Lane:** shard logico do ledger do Nexus com seu proprio set de validadores e backlog de execucao. Identificado por um `LaneId` estavel.
- **Data Space:** bucket de governanca que agrupa uma ou mais lanes que compartilham politicas de compliance, routing e settlement.
- **Lane Manifest:** metadata controlada pela governanca que descreve validadores, politica de DA, token de gas, regras de settlement e permissoes de routing.
- **Global Commitment:** proof emitida por uma lane que resume novos roots de estado, dados de settlement e transferencias cross-lane opcionais. O anel NPoS global ordena commitments.

## Taxonomia de lanes

Os tipos de lane descrevem de forma canonica sua visibilidade, superficie de governanca e hooks de settlement. A geometria de configuracao (`LaneConfig`) captura esses atributos para que nos, SDKs e tooling possam raciocinar sobre o layout sem logica bespoke.

| Tipo de lane | Visibilidade | Membresia de validadores | Exposicao WSV | Governanca por default | Politica de settlement | Uso tipico |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Replica de estado completa | SORA Parliament | `xor_global` | Ledger publico base |
| `public_custom` | public | Permissionless ou stake-gated | Replica de estado completa | Modulo ponderado por stake | `xor_lane_weighted` | Aplicacoes publicas de alto throughput |
| `private_permissioned` | restricted | Set fixo de validadores (aprovado pela governanca) | Commitments e proofs | Federated council | `xor_hosted_custody` | CBDC, workloads de consorcio |
| `hybrid_confidential` | restricted | Membresia mista; envolve ZK proofs | Commitments + disclosure seletiva | Modulo de dinheiro programavel | `xor_dual_fund` | Dinheiro programavel com preservacao de privacidade |

Todos os tipos de lane devem declarar:

- Alias de dataspace - agrupamento legivel por humanos que vincula politicas de compliance.
- Handle de governanca - identificador resolvido via `Nexus.governance.modules`.
- Handle de settlement - identificador consumido pelo settlement router para debitar buffers XOR.
- Metadata opcional de telemetria (descricao, contato, dominio de negocio) exposta via `/status` e dashboards.

## Geometria de configuracao de lanes (`LaneConfig`)

`LaneConfig` e a geometria runtime derivada do catalogo de lanes validado. Ele nao substitui os manifests de governanca; em vez disso fornece identificadores de storage deterministas e dicas de telemetria para cada lane configurada.

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

- `LaneConfig::from_catalog` recalcula a geometria quando a configuracao e carregada (`State::set_nexus`).
- Aliases sao sanitizados em slugs minusculos; caracteres nao alfanumericos consecutivos colapsam em `_`. Se o alias produzir um slug vazio, usamos `lane{id}`.
- `shard_id` e derivado da key de metadata `da_shard_id` (default `lane_id`) e dirige o journal de cursor de shard persistido para manter o replay de DA determinista entre restarts/resharding.
- Prefixos de chave garantem que o WSV mantenha ranges de chave por lane disjuntos mesmo quando o mesmo backend e compartilhado.
- Nomes de segmentos Kura sao deterministas entre hosts; auditores podem verificar diretorios de segmento e manifests sem tooling bespoke.
- Segmentos de merge (`lane_{id:03}_merge`) guardam as ultimas roots de merge-hint e commitments de estado global para aquela lane.

## Particionamento de world-state

- O world state logico do Nexus e a uniao de espacos de estado por lane. Lanes publicas persistem estado completo; lanes privadas/confidential exportam roots Merkle/commitment para o merge ledger.
- O armazenamento MV prefixa cada chave com o prefixo de 4 bytes de `LaneConfigEntry::key_prefix`, gerando chaves como `[00 00 00 01] ++ PackedKey`.
- Tabelas compartilhadas (accounts, assets, triggers, registros de governanca) armazenam entradas agrupadas por prefixo de lane, mantendo os range scans deterministas.
- A metadata do merge-ledger reflete o mesmo layout: cada lane escreve roots de merge-hint e roots de estado global reduzido em `lane_{id:03}_merge`, permitindo retencao ou eviction direcionada quando uma lane se retira.
- Indices cross-lane (account aliases, asset registries, governance manifests) armazenam prefixos de lane explicitos para que operadores reconciliem entradas rapidamente.
- **Politica de retencao** - lanes publicas retencem corpos de bloco completos; lanes apenas de commitments podem compactar corpos antigos apos checkpoints porque commitments sao autoritativos. Lanes confidential guardam journals cifrados em segmentos dedicados para nao bloquear outros workloads.
- **Tooling** - utilitarios de manutencao (`kagami`, comandos admin de CLI) devem referenciar o namespace com slug ao expor metricas, labels Prometheus ou ao arquivar segmentos Kura.

## Routing e APIs

- Endpoints Torii REST/gRPC aceitam um `lane_id` opcional; ausencia implica `lane_default`.
- SDKs expoem seletores de lane e mapeiam aliases amigaveis para `LaneId` usando o catalogo de lanes.
- Regras de routing operam sobre o catalogo validado e podem escolher lane e dataspace. `LaneConfig` fornece aliases amigaveis para telemetria em dashboards e logs.

## Settlement e fees

- Cada lane paga fees XOR ao set global de validadores. Lanes podem coletar tokens de gas nativos, mas devem fazer escrow de equivalentes XOR junto com commitments.
- Provas de settlement incluem montante, metadata de conversao e prova de escrow (por exemplo, transferencia para o vault global de fees).
- O settlement router unificado (NX-3) debita buffers usando os mesmos prefixos de lane, assim a telemetria de settlement se alinha com a geometria de storage.

## Governance

- Lanes declaram seu modulo de governanca via catalogo. `LaneConfigEntry` carrega o alias e o slug originais para manter telemetria e audit trails legiveis.
- O registry do Nexus distribui lane manifests assinados que incluem `LaneId`, binding de dataspace, handle de governanca, handle de settlement e metadata.
- Hooks de runtime-upgrade continuam aplicando politicas de governanca (`gov_upgrade_id` por default) e registram diffs via telemetry bridge (eventos `nexus.config.diff`).

## Telemetria e status

- `/status` expoe aliases de lane, bindings de dataspace, handles de governanca e perfis de settlement, derivados do catalogo e do `LaneConfig`.
- Metricas do scheduler (`nexus_scheduler_lane_teu_*`) exibem aliases/slugs para que operadores mapeiem backlog e pressao de TEU rapidamente.
- `nexus_lane_configured_total` conta o numero de entradas de lane derivadas e e recalculado quando a configuracao muda. A telemetria emite diffs assinados quando a geometria de lanes muda.
- Gauges de backlog de dataspace incluem metadata de alias/descricao para ajudar operadores a associar pressao de fila a dominios de negocio.

## Configuracao e tipos Norito

- `LaneCatalog`, `LaneConfig`, e `DataSpaceCatalog` vivem em `iroha_data_model::nexus` e fornecem estruturas compativeis com Norito para manifests e SDKs.
- `LaneConfig` vive em `iroha_config::parameters::actual::Nexus` e e derivado automaticamente do catalogo; nao requer encoding Norito porque e um helper interno runtime.
- A configuracao voltada ao usuario (`iroha_config::parameters::user::Nexus`) continua aceitando descritores declarativos de lane e dataspace; o parsing agora deriva a geometria e rejeita aliases invalidos ou IDs de lane duplicados.

## Trabalho pendente

- Integrar updates do settlement router (NX-3) com a nova geometria para que debitos e recibos de buffer XOR sejam etiquetados por slug de lane.
- Estender tooling admin para listar column families, compactar lanes aposentadas e inspecionar logs de blocos por lane usando o namespace com slug.
- Finalizar o algoritmo de merge (ordering, pruning, conflict detection) e anexar fixtures de regressao para replay cross-lane.
- Adicionar hooks de compliance para whitelists/blacklists e politicas de dinheiro programavel (acompanhado sob NX-12).

---

*Esta pagina continuara rastreando follow-ups de NX-1 conforme NX-2 ate NX-18 aterrissarem. Por favor traga perguntas em aberto para `roadmap.md` ou o tracker de governanca para que o portal fique alinhado com os docs canonicos.*
