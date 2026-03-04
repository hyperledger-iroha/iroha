---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/lane-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo nexus-lane
título: Modelo de pistas do Nexus
descrição: Taxonomia lógica de pistas, geometria de configuração e regras de fusão de estado-mundo para Sora Nexus.
---

# Modelo de pistas do Nexus e particionamento de WSV

> **Status:** entregavel NX-1 - taxonomia de pistas, geometria de configuração e layout de armazenamento prontos para implementação.  
> **Proprietários:** Nexus GT Central, GT de Governança  
> **Referência de roadmap:** NX-1 em `roadmap.md`

Esta página do portal reflete o breve canônico `docs/source/nexus_lanes.md` para que operadores do Sora Nexus, proprietários de SDK e revisores possam ler um guia de pistas sem entrar na árvore mono-repo. A arquitetura alvo mantém o estado mundial determinista enquanto permite que espaços de dados (lanes) indivíduos executem conjuntos de validadores públicos ou privados com cargas de trabalho isoladas.

## Conceitos

- **Lane:** shard lógico do ledger do Nexus com seu próprio conjunto de validadores e backlog de execução. Identificado por um estavel `LaneId`.
- **Data Space:** bucket de governança que agrupa uma ou mais vias que reúnem políticas de compliance, roteamento e liquidação.
- **Lane Manifest:** metadados controlados pela governança que descrevem validadores, política de DA, token de gás, regras de liquidação e permissões de roteamento.
- **Compromisso Global:** comprovante emitido por uma via que resume novas raízes de estado, dados de liquidação e transferências cross-lane indiretamente. O anel NPoS ordena compromissos globais.

## Taxonomia de pistas

Os tipos de via descrevem de forma canônica sua visibilidade, superfície de governança e ganchos de liquidação. A geometria de configuração (`LaneConfig`) captura esses atributos para que nós, SDKs e ferramentas possam raciocinar sobre o layout sem lógica sob medida.

| Tipo de pista | Visibilidade | Membros de validadores | Exposição WSV | Governança por padrão | Política de liquidação | Uso típico |
|-----------|------------|-----------|-------------|--------------------|-------------------|-------------|
| `default_public` | público | Sem permissão (participação global) | Réplica de estado completo | SORA Parlamento | `xor_global` | Base pública do razão |
| `public_custom` | público | Sem permissão ou protegido por estacas | Réplica de estado completo | Módulo ponderado por estaca | `xor_lane_weighted` | Aplicações públicas de alto rendimento |
| `private_permissioned` | restrito | Set fixo de validadores (aprovado pela governança) | Compromissos e comprovações | Conselho Federado | `xor_hosted_custody` | CBDC, cargas de trabalho de consórcio |
| `hybrid_confidential` | restrito | Membresia mista; envolve provas ZK | Compromissos + divulgação seletiva | Módulo de dinheiro programável | `xor_dual_fund` | Dinheiro programavel com preservação de privacidade |

Todos os tipos de pista devem declarar:

- Alias de dataspace - agrupamento legítimo para humanos que vincula políticas de conformidade.
- Handle de governança - identificador resolvido via `Nexus.governance.modules`.
- Handle de liquidação - identificador consumido pelo roteador de liquidação para debitar buffers XOR.
- Metadados opcionais de telemetria (descrição, contato, domínio de negócio) exibidos via `/status` e dashboards.

## Geometria de configuração de pistas (`LaneConfig`)

`LaneConfig` e uma geometria runtime derivada do catálogo de pistas validado. Ele não substitui os manifestos de governança; em vez disso fornece identificadores de deterministas de armazenamento e dicas de telemetria para cada pista configurada.

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

- `LaneConfig::from_catalog` recalcula a geometria quando a configuração e são incluídas (`State::set_nexus`).
- Aliases são sanitizados em lesmas minúsculas; caracteres não alfanuméricos consecutivos colapsam em `_`. Se o alias produzir um slug vazio, `lane{id}`.
- `shard_id` e derivado da chave de metadados `da_shard_id` (padrão `lane_id`) e dirige o diário do cursor do shard persistido para manter o replay do DA determinista entre reinicializações/resharding.
- Prefixos de chave garantem que o WSV mantenha intervalos de chave por via disjuntos mesmo quando o mesmo backend e compartilhado.
- Nomes de segmentos Kura são deterministas entre hosts; auditores podem verificar diretórios de segmentos e manifestos sem ferramentas sob medida.
- Segmentos de merge (`lane_{id:03}_merge`) guardam as últimas raízes de merge-hint e compromissos de estado global para aquela lane.

## Particionamento do estado mundial- O estado mundial lógico do Nexus e a união de espaços de estado por pista. Lanes publicas persistem estado completo; faixas privadas/confidenciais exportam raízes Merkle/commitment para o merge ledger.
- O armazenamento MV prefixa cada chave com o prefixo de 4 bytes de `LaneConfigEntry::key_prefix`, gerando chaves como `[00 00 00 01] ++ PackedKey`.
- Tabelas compartilhadas (contas, ativos, gatilhos, registros de governança) armazenam entradas agrupadas por prefixo de faixa, mantendo os range scans deterministas.
- Os metadados do merge-ledger refletem o mesmo layout: cada lane escreve raízes de merge-hint e raízes de estado global limitadas em `lane_{id:03}_merge`, permitindo retencao ou despejo direcionado quando uma lane se retira.
- Índices cross-lane (aliases de contas, registros de ativos, manifestos de governança) armazenam prefixos de lane explícitos para que os operadores reconciliem entradas rapidamente.
- **Politica de retencao** - lanes publicas retencem corpos de bloco completos; faixas apenas de compromissos podem compactar corpos antigos após pontos de verificação porque compromissos são autoritativos. Faixas confidenciais guardam periódicos cifrados em segmentos dedicados para não bloquear outras cargas de trabalho.
- **Tooling** - utilitários de manutenção (`kagami`, comandos admin de CLI) devem referenciar o namespace com slug ao exportar métricas, rótulos Prometheus ou ao arquivar segmentos Kura.

## Roteamento e APIs

- Endpoints Torii REST/gRPC aceitam um `lane_id` opcional; ausência implica `lane_default`.
- SDKs expoem seletores de lane e mapeiam aliases amigos para `LaneId` usando o catálogo de lanes.
- Regras de roteamento operam sobre o catálogo validado e podem escolher pista e espaço de dados. `LaneConfig` fornece aliases amigos para telemetria em dashboards e logs.

## Taxas de liquidação e

- Cada pista paga taxas XOR ao conjunto global de validadores. As faixas podem coletar tokens de gás nativos, mas devem fazer garantia de equivalentes XOR junto com compromissos.
- As provas de liquidação incluem montantes, metadados de conversação e prova de garantia (por exemplo, transferência para o cofre global de taxas).
- O roteador de liquidação unificado (NX-3) buffers de débito usando os mesmos prefixos de pista, assim a telemetria de liquidação se alinha com a geometria de armazenamento.

## Governança

- Faixas declaram seu módulo de governança via catálogo. `LaneConfigEntry` carrega o alias e o slug originais para manter telemetria e trilhas de auditoria legíveis.
- O registro do Nexus distribui manifestos contratados que incluem `LaneId`, vinculação de espaço de dados, identificador de governança, identificador de liquidação e metadados.
- Hooks de runtime-upgrade continuam aplicando políticas de governança (`gov_upgrade_id` por padrão) e registram diffs via telemetry bridge (eventos `nexus.config.diff`).

## Telemetria e status

- `/status` expõe aliases de lane, vinculações de dataspace, identificadores de governança e perfis de liquidação, resultados do catálogo e do `LaneConfig`.
- Métricas do agendador (`nexus_scheduler_lane_teu_*`) exibem aliases/slugs para que os operadores mapeiem backlog e pressão de TEU rapidamente.
- `nexus_lane_configured_total` conta o número de entradas da pista derivada e é recalculado quando a configuração muda. A telemetria emite diferenças assinadas quando a geometria das pistas muda.
- Medidores de backlog de espaço de dados incluem metadados de alias/descrição para ajudar operadores a associar pressão de fila a domínios de negócio.

## Configuração e tipos Norito

- `LaneCatalog`, `LaneConfig`, e `DataSpaceCatalog` vivem em `iroha_data_model::nexus` e fornecem estruturas compatíveis com Norito para manifestos e SDKs.
- `LaneConfig` vive em `iroha_config::parameters::actual::Nexus` e é derivado automaticamente do catálogo; Não requer codificação Norito porque é um helper interno runtime.
- A configuração externa ao usuário (`iroha_config::parameters::user::Nexus`) continua aceitando descritores declarativos de pista e espaço de dados; o parsing agora deriva da geometria e rejeita aliases inválidos ou IDs de pista duplicados.

##Trabalho pendente- Integrar atualizações do roteador de liquidação (NX-3) com uma nova geometria para que débitos e recibos de buffer XOR sejam etiquetados por slug de lane.
- Estender tooling admin para listar famílias de colunas, compactar lanes aposentadas e operar logs de blocos por lane usando o namespace com slug.
- Finalizar o algoritmo de mesclagem (ordenação, remoção, detecção de conflitos) e fixação de fixtures de regressão para replay cross-lane.
- Adicionar ganchos de compliance para whitelists/blacklists e políticas de dinheiro programavel (acompanhado sob NX-12).

---

*Esta página continua rastreando follow-ups de NX-1 conforme NX-2 até NX-18 aterrissarem. Por favor, traga perguntas em aberto para `roadmap.md` ou o tracker de governança para que o portal fique alinhado com os documentos canônicos.*